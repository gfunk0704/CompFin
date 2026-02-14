
pub struct InterestRateS {
    position: Position,
    pay_leg_nominal_generator: Rc<dyn NominalGenerator>,
    pay_leg_characters: Rc<dyn LegCharacters>,
    pay_leg_flow_observers: Vec<FlowObserver>,
    pay_leg_capitalization_flows: Vec<CapitalizationFlow>,
    pay_leg_forward_curve: RefCell<Option<Rc<dyn InterestRateCurve>>>,
    receive_leg_nominal_generator: Rc<dyn NominalGenerator>,
    receive__leg_characters: Rc<dyn LegCharacters>,
    receive__leg_flow_observers: Vec<FlowObserver>,
    receive__leg_capitalization_flows: Vec<CapitalizationFlow>,
    receive_leg_forward_curve: RefCell<Option<Rc<dyn InterestRateCurve>>>,
    profit_and_loss_market: Rc<dyn Market>,
    discount_curve: RefCell<Option<Rc<dyn InterestRateCurve>>>,
    curve_name_map: HashMap<String, String>
}